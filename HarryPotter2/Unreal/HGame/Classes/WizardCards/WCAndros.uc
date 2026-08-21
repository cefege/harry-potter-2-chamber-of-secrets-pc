//===============================================================================
//  #13 Andros the Invincible
//===============================================================================

class  WCAndros extends silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardAndrosTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Androssmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardAndrosBigTexture FILE=TEXTURES\menu\Folio\Cards\Androsbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Andros the Invincible";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=13
    skin(0)=Texture'HProps.skins.WizardCardAndrosTex0'
	textureBig=Texture'WizCardAndrosBigTexture'
	strDescriptionId="WizCard_0051"
}
