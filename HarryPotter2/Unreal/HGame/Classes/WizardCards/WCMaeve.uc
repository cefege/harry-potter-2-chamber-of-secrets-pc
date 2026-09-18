//===============================================================================
//  #71 Queen Maeve
//===============================================================================

class  WCMaeve extends silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardMaeveTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Maevesmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardMaeveBigTexture FILE=TEXTURES\menu\Folio\Cards\Maevebig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF


function PostBeginPlay()
{
	WizardName = "Queen Maeve";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=71
    skin(0)=Texture'HProps.skins.WizardCardMaeveTex0'
	textureBig=Texture'WizCardMaeveBigTexture'
	strDescriptionId="WizCard_0065"
}
