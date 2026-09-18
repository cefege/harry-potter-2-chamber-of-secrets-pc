//===============================================================================
//  #59 Gregory the Smarmy
//===============================================================================

class  WCGregory extends silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardGregoryTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Gregorysmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardGregoryBigTexture FILE=TEXTURES\menu\Folio\Cards\Gregorybig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Gregory the Smarmy";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=59
    skin(0)=Texture'HProps.skins.WizardCardGregoryTex0'
	textureBig=Texture'WizCardGregoryBigTexture'
	strDescriptionId="WizCard_0095"
}
