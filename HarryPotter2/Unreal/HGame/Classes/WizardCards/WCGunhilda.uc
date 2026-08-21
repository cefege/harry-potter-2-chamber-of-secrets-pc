//===============================================================================
//  #9 Gunhilda of Gorsemoor
//===============================================================================

class  WCGunhilda extends bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardGunhildaTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Gunhildasmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardGunhildaBigTexture FILE=TEXTURES\menu\Folio\Cards\Gunhildabig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF


function PostBeginPlay()
{
	WizardName = "Gunhilda of Gorsemoor";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=9
    skin(0)=Texture'HProps.skins.WizardCardGunhildaTex0'
	textureBig=Texture'WizCardGunhildaBigTexture'
	strDescriptionId="WizCard_0047"
}
